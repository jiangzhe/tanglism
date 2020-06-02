// 定义次级别走势相关函数
// 依赖jquery, jquery-ui, d3, tanglism-common, tanglism-kline
export const subtrend = {
    data,
    table,
    clear_table,
    draw,
    outdate
};

import { kline, display_tooltip, hide_tooltip } from './tanglism-kline.js';

const _data = [];
var _outdate = true;

function data(input) {
    if (input) {
      while (_data.length > 0) { _data.pop(); }
      for (var i = 0; i < input.length; i++) {
        _data.push(input[i]);
      }
      _outdate = false;
      return;
    }
    return _data;
}

function table() {
    // 创建表格
    if (!d3.select("#st_table").empty()) {
      d3.select("#st_table").remove();
    }
    var table = d3.select("#st_container").append("table")
      .attr("id", "st_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["类型", "起始时刻", "起始价格", "终止时刻", "终止价格", "走向"])
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
          d.level === 1 ? "笔" : "线段",
          d.start_ts, 
          d.start_price, 
          d.end_ts, 
          d.end_price, 
          parseFloat(d.start_price) < parseFloat(d.end_price) ? "上升" : "下降"
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
}

function clear_table() {
    // 删除表格
    d3.select("#st_table").remove();
}

function draw(config) {
    // 是否在图中显示线段
    var subtrend_draw_check = $("#subtrend_draw").is(":checked");
    if (!subtrend_draw_check) {
      return;
    }
    if (_outdate) {
      console.log("subtrend outdate");
      return;
    }
    var conf = config || kline.conf();
    // 无K线图，直接退出
    if (d3.select("#k_lines").empty()) {
      return;
    }
    // 无K线数据或线段数据，直接退出
    if (kline.data().length == 0 || _data.length == 0) {
      return;
    }
    // 双指针，查询笔所在柱状图，并添加序列号
    var ki = 0;
    var si = 0;
    var start_match = false;
    var kdata = kline.data();
    while (si < _data.length && ki < kdata.length) {
      var st = _data[si];
      var k = kdata[ki];
      if (!start_match && st.start_ts === k.ts) {
        // 起点序列号
        st.start_id = ki;
        // 将start_match置为true
        start_match = true;
      } else if (st.end_ts === k.ts) {
        // 终点序列号
        st.end_id = ki;
        // 仅递增线段，下一线段起点应与前一线段终点一致，需复用ki
        si++;
        start_match = false;
      } else {
        // 未匹配到，K线号递增
        ki++;
      }
    }

    // 过滤出所有匹配上的线段
    var subtrends = [];
    for (var i = 0; i < _data.length; i++) {
      var item = _data[i];
      if (item.hasOwnProperty("start_id") && item.hasOwnProperty("end_id")) {
        subtrends.push(item);
      }
    }
    var svg = d3.select("#k_lines");
    svg.selectAll("line.subtrend")
        .data(subtrends)
        .enter()
        .append("line")
        .attr("class", "subtrend")
        .attr("x1", function(d, i) {
            return d.start_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("x2", function(d, i) {
            return d.end_id * conf.bar_width + conf.bar_inner_width / 2;
        })
        .attr("y1", function(d) {
            return conf.h - conf.yscale(parseFloat(d.start_price));
        })
        .attr("y2", function(d) {
            return conf.h - conf.yscale(parseFloat(d.end_price));
        })
        // .attr("stroke", function(d) {
        //   return d.level === 1 ? "violet" : "purple";
        // })
        // .attr("stroke-width", function(d) {
        //   return d.level === 1 ? 1 : 2;
        // })
        .attr("stroke", "purple")
        .attr("stroke-width", 2)
        .on("mouseover", function(d) {
          var start_dt = d.start_ts.substring(0, 10);
          var start_tm = d.start_ts.substring(11, 16);
          var end_dt = d.end_ts.substring(0, 10);
          var end_tm = d.end_ts.substring(11, 16);
          const innerHtml = "开始日期：" + start_dt + "<br/>" + 
            "开始时刻：" + start_tm + "<br/>" + 
            "开始价格：" + d.start_price + "<br/>" +
            "结束日期：" + end_dt + "<br/>" +
            "结束时刻：" + end_tm + "<br/>" +
            "结束价格：" + d.end_price;
          display_tooltip(d3.event, innerHtml);
          // 加粗
          d3.select(this).attr("stroke-width", function(d) {
            return d.level === 1 ? 2 : 4;
          });
        })
        .on("mouseout", function(d) {
          hide_tooltip();
          // 还原
          d3.select(this).attr("stroke-width", function(d) {
            return d.level === 1 ? 1 : 2;
          });
        });
};

function outdate() {
  _outdate = true;
}