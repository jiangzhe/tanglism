// 定义笔相关函数
// 依赖jquery, jquery-ui, d3, tanglism-common, tanglism-kline
export const stroke = {
    data,
    clear_data,
    table,
    clear_table,
    draw,
    outdate
};

import { kline, display_tooltip, hide_tooltip } from './tanglism-kline.js';

// 笔数据
const _data = [];
var _outdate = true;

function data(input) {
    if (input) {
      while(_data.length > 0) { _data.pop(); }
      for (var i = 0; i < input.length; i++) {
        _data.push(input[i]);
      }
      _outdate = false;
      return;
    }
    return _data;
};

function clear_data() {
  while(_data.length > 0) { _data.pop(); }
  _outdate = true;
}

// 仅在K线图完成后调用
function draw(config) {
    var stroke_draw_check = $("#stroke_draw").is(":checked");
    if (!stroke_draw_check) {
      return;
    }
    if (_outdate) {
      console.log("stroke outdate");
      return;
    }

    var conf = config || kline.conf();
    // 无K线图，直接退出
    if (d3.select("#k_lines").empty()) {
      return;
    }
    // 无K线数据或笔数据，直接退出
    if (kline.data().length == 0 || _data.length == 0) {
      return;
    }
    // 双指针，查询笔所在柱状图，并添加序列号
    var ki = 0;
    var si = 0;
    var kdata = kline.data();
    while (si < _data.length && ki < kdata.length) {
      var sk = _data[si];
      var k = kdata[ki];
      if (sk.start_pt.extremum_ts === k.ts) {
        // 起点序列号
        sk.start_id = ki;
        // 递增笔
        ki++;
      } else if (sk.end_pt.extremum_ts === k.ts) {
        // 终点序列号
        sk.end_id = ki;
        // 仅递增笔，下一笔起点应与前一笔终点一致，需复用ki
        si++;
      } else {
        // 未匹配到，K线号递增
        ki++;
      }
    }

    // 过滤出所有匹配上的笔
    var strokes = [];
    for (var i = 0; i < _data.length; i++) {
      var item = _data[i];
      if (item.hasOwnProperty("start_id") && item.hasOwnProperty("end_id")) {
        strokes.push(item);
      }
    }
    var svg = d3.select("#k_lines");
    svg.selectAll("line.stroke")
        .data(strokes)
        .enter()
        .append("line")
        .attr("class", "stroke")
        .attr("x1", function(d, i) {
            return d.start_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("x2", function(d, i) {
            return d.end_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("y1", function(d) {
            return conf.h - conf.yscale(parseFloat(d.start_pt.extremum_price));
        })
        .attr("y2", function(d) {
            return conf.h - conf.yscale(parseFloat(d.end_pt.extremum_price));
        })
        .attr("stroke", "blue")
        .attr("stroke-width", 1)
        .on("mouseover", function(d) {
          var start_dt = d.start_pt.extremum_ts.substring(0, 10);
          var start_tm = d.start_pt.extremum_ts.substring(11, 16);
          var end_dt = d.end_pt.extremum_ts.substring(0, 10);
          var end_tm = d.end_pt.extremum_ts.substring(11, 16);
          const innerHtml = "开始日期：" + start_dt + "<br/>" + 
            "开始时刻：" + start_tm + "<br/>" + 
            "开始价格：" + d.start_pt.extremum_price + "<br/>" +
            "结束日期：" + end_dt + "<br/>" +
            "结束时刻：" + end_tm + "<br/>" +
            "结束价格：" + d.end_pt.extremum_price;
          display_tooltip(d3.event, innerHtml);
          // 加粗
          d3.select(this).attr("stroke-width", 2);
        })
        .on("mouseout", function(d) {
          hide_tooltip();
          // 还原
          d3.select(this).attr("stroke-width", 1);
        });
};

// todo更名
function clear_table() {
    d3.select("#sk_table").remove();
}

function table() {
    // 创建表格
    if (!d3.select("#sk_table").empty()) {
      d3.select("#sk_table").remove();
    }
    var table = d3.select("#sk_container").append("table")
      .attr("id", "sk_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["起始时刻", "起始价格", "终止时刻", "终止价格", "走向"])
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold");
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(_data)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        return [
          d.start_pt.extremum_ts, 
          d.start_pt.extremum_price, 
          d.end_pt.extremum_ts, 
          d.end_pt.extremum_price, 
          parseFloat(d.start_pt.extremum_price) < parseFloat(d.end_pt.extremum_price) ? "上升" : "下降"
        ];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
};

function outdate() {
  _outdate = true;
}