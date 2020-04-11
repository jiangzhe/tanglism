var charts = (function(){
  // K线数据
  var kdata = [];
  // 分型数据
  var pdata = [];
  // 笔数据
  var skdata = [];
  // 线段数据
  var sgdata = [];

  var kdata_fn = function(input) {
    if (input) {
      while(kdata.length > 0) { kdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        kdata.push(input[i]);
      }
      return;
    }
    return kdata;
  };
  var kdata_clear = function() {
    // 删除tooltip
    d3.select("#k_tooltip").remove();
    // 删除svg
    d3.select("#k_lines").remove();
    // 删除标题
    d3.select("#k_lines_title").remove();
  };
  var kdata_draw = function() {
    // 单柱宽度，包含间隔
    var bar_width = parseInt($("#bar_width option:selected").text());
    var bar_padding = Math.max(bar_width / 3, 4);
    var bar_inner_width = bar_width - bar_padding;
    // 整体宽度
    var w = bar_width * kdata.length;
    // 整体高度
    var h = parseInt($("#chart_height option:selected").text());
    // 最大最小值统计
    var price_max = d3.max(kdata, function(d) {
      return d.high;
    });
    var price_min = d3.min(kdata, function(d) {
        return d.low;
    });
    // 缩放比例
    var yscale = d3.scaleLinear([price_min, price_max], [0, h]);
    // 创建标题
    if (!d3.select("#k_lines_title").empty()) {
      d3.select("#k_lines_title").remove();
    }
    d3.select("#k_container").append("div").attr("id", "k_lines_title")
      .text("K线图");
    // 创建K线图
    if (!d3.select("#k_lines").empty()) {
      // 如存在则删除
      d3.select("#k_lines").remove();
    }
    var svg = d3.select("#k_container")
      .append("svg")
      .attr("id", "k_lines")
      .attr("width", w)
      .attr("height", h);
    // 创建单例提示
    if (!d3.select("#k_tooltip").empty()) {
      d3.select("#k_tooltip").remove();
    }
    var tooltip = d3.select("#k_container")
      .append("div")
      .attr("class", "tooltip")
      .style("opacity", 0);
    // 画图
    svg.selectAll("rect").data(kdata).enter().append("rect")
        .attr("x", function(d, i) {
            return i * bar_width;
        })
        .attr('y', function(d, i) {
            return h - yscale(d3.max([d.open, d.close]));
        })
        .attr('width', bar_inner_width)
        .attr("height", function(d) {
          // 当开盘与收盘价相等时，至少保证1的高度
          return Math.max(1, Math.abs(yscale(d.open) - yscale(d.close)));
        })
        .attr("fill", function(d) {
            if (d.open < d.close) return "red";
            return "green";
        })
        .on("mouseover", function(d) {
          tooltip.transition()
            .duration(200)
            .style("opacity", 0.9);
          var dt = d.date.substring(0, 10);
          var tm = d.date.substring(11, 16);
          tooltip
            .html(
              "日期：" + dt + "<br/>" + 
              "时刻：" + tm + "<br/>" +
              "开盘价：" + d.open + "<br/>" +
              "收盘价：" + d.close + "<br/>" +
              "最高价：" + d.high + "<br/>" + 
              "最低价：" + d.low)
            .style("left", (d3.event.pageX + 30) + "px")
            .style("top", (d3.event.pageY + 30) + "px");
        })
        .on("mouseout", function(d) {
          tooltip.transition()
            .duration(500)
            .style("opacity", 0);
        });
  
    // 构造中线
    svg.selectAll("line")
        .data(kdata)
        .enter()
        .append("line")
        .attr("x1", function(d, i) {
            return i * bar_width + bar_inner_width / 2;
        })
        .attr("x2", function(d, i) {
            return i * bar_width + bar_inner_width / 2;
        })
        .attr("y1", function(d) {
            return h - yscale(d.high);
        })
        .attr("y2", function(d) {
            return h - yscale(d.low);
        })
        .attr("stroke", function(d) {
            if (d.open < d.close) return "red";
            return "green";
        });
  };
  var pdata_fn = function(input) {
    if (input) {
      while(pdata.length > 0) { pdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        pdata.push(input[i]);
      }
      return;
    }
    return pdata;
  };
  var pdata_clear = function() {
    // 删除表格
    d3.select("#p_table").remove();
    // 删除标题
    d3.select("#p_table_title").remove();
  };
  var pdata_table = function() {
    var p_check = $("#p_check").is(":checked");
    if (!p_check) {
      pdata_clear();
      return;
    }
    // 创建标题
    if (!d3.select("#p_table_title").empty()) {
      d3.select("#p_table_title").remove();
    }
    d3.select("#p_container").append("div")
      .attr("id", "p_table_title")
      .text("分型");
    // 创建表格
    if (!d3.select("#p_table").empty()) {
      d3.select("#p_table").remove();
    }
    var table = d3.select("#p_container").append("table")
      .attr("id", "p_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["峰值时刻", "峰值价格", "起始时刻", "结束时刻", "K线数目", "类型"])
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
      .data(pdata)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        return [d.extremum_ts, d.extremum_price, d.start_ts, d.end_ts, d.n, d.top ? "顶分型" : "底分型"];
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
  var skdata_fn = function(input) {
    if (input) {
      while(skdata.length > 0) { skdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        skdata.push(input[i]);
      }
      return;
    }
    return skdata;
  };
  var skdata_clear = function() {
    // 删除表格
    d3.select("#sk_table").remove();
    // 删除标题
    d3.select("#sk_table_title").remove();
  };
  var skdata_table = function() {
    var sk_check = $("#sk_check").is(":checked");
    if (!sk_check) {
      skdata_clear();
      return;
    }
    // 创建标题
    if (!d3.select("#sk_table_title").empty()) {
      d3.select("#sk_table_title").remove();
    }
    d3.select("#sk_container").append("div")
      .attr("id", "sk_table_title")
      .text("笔");
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
      .data(skdata)
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
  var sgdata_fn = function(input) {
    if (input) {
      while(sgdata.length > 0) { sgdata.pop(); }
      for (var i = 0; i < input.length; i++) {
        sgdata.push(input[i]);
      }
      return;
    }
    return sgdata;
  };
  var sgdata_clear = function() {
    // 删除表格
    d3.select("#sg_table").remove();
    // 删除标题
    d3.select("#sg_table_title").remove();
  };
  var sgdata_table = function() {
    var sg_check = $("#sg_check").is(":checked");
    if (!sg_check) {
      sgdata_clear();
      return;
    }
    // 创建标题
    if (!d3.select("#sg_table_title").empty()) {
      d3.select("#sg_table_title").remove();
    }
    d3.select("#sg_container").append("div")
      .attr("id", "sg_table_title")
      .text("线段");
    // 创建表格
    if (!d3.select("#sg_table").empty()) {
      d3.select("#sg_table").remove();
    }
    var table = d3.select("#sg_container").append("table")
      .attr("id", "sg_table")
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
      .data(sgdata)
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

  return {
    kdata: kdata_fn,
    kdata_draw: kdata_draw,
    kdata_clear: kdata_clear,
    pdata: pdata_fn,
    pdata_table: pdata_table,
    pdata_clear: pdata_clear,
    skdata: skdata_fn,
    skdata_table: skdata_table,
    skdata_clear: skdata_clear,
    sgdata: sgdata_fn,
    sgdata_table: sgdata_table,
    sgdata_clear: sgdata_clear
  };
})();

$(document).ready(function() {
  $("#input_stock_code").autocomplete({
    source: function(req, callback) {
      $.ajax({
        url: "api/v1/keyword-stocks?keyword=" + encodeURIComponent(req.term),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          callback($.map(resp.content, function(item){
            return {
              value: item.code,
              label: item.code + " " + item.display_name
            };
          }));
        },
        error: function(err) {
          console.log("ajax error on search stock", err);
          callback([]);
        }
      })
    }
  });
  $("#input_start_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: -1
  });
  $("#input_end_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: -1
  });
  var pdata_ajax = function(tick, code, start_dt, end_dt) {
    $.ajax({
      url: "api/v1/tanglism/partings/" + encodeURIComponent(code)
        + "/ticks/" + encodeURIComponent(tick) 
        + "?start_dt=" + encodeURIComponent(start_dt) 
        + "&end_dt=" + encodeURIComponent(end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        charts.pdata(resp.data);
        charts.pdata_table();
      },
      error: function(err) {
        console.log("ajax error on query partings", err);
        charts.pdata_clear();
      }
    });
  };

  var skdata_ajax = function(tick, code, start_dt, end_dt) {
    $.ajax({
      url: "api/v1/tanglism/strokes/" + encodeURIComponent(code)
        + "/ticks/" + encodeURIComponent(tick) 
        + "?start_dt=" + encodeURIComponent(start_dt) 
        + "&end_dt=" + encodeURIComponent(end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        charts.skdata(resp.data),
        charts.skdata_table();
      },
      error: function(err) {
        console.log("ajax error on query strokes", err);
        charts.skdata_clear();
      }
    });
  };

  var sgdata_ajax = function(tick, code, start_dt, end_dt) {
    $.ajax({
      url: "api/v1/tanglism/segments/" + encodeURIComponent(code)
        + "/ticks/" + encodeURIComponent(tick) 
        + "?start_dt=" + encodeURIComponent(start_dt) 
        + "&end_dt=" + encodeURIComponent(end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        charts.sgdata(resp.data),
        charts.sgdata_table();
      },
      error: function(err) {
        console.log("ajax error on query strokes", err);
        charts.sgdata_clear();
      }
    });
  }

  $("#stock_submission").click(function() {
    var input_stock_code = $("#input_stock_code").val();
    var input_start_dt = $("#input_start_dt").val();
    var input_end_dt = $("#input_end_dt").val();
    var input_tick = $("#input_tick option:selected").text();
    
    var p_check = $("#p_check").is(":checked");
    var sk_check = $("#sk_check").is(":checked");
    var sg_check = $("#sg_check").is(":checked");
    $.ajax({
      url: "api/v1/stock-prices/" + encodeURIComponent(input_stock_code)
        + "/ticks/" + encodeURIComponent(input_tick) + "?start_dt="
        + encodeURIComponent(input_start_dt) + "&end_dt="
        + encodeURIComponent(input_end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        var kdata = $.map(resp.data, function(item) {
          return {
            date: item.ts,
            open: parseFloat(item.open),
            close: parseFloat(item.close),
            high: parseFloat(item.high),
            low: parseFloat(item.low)
          };
        });
        charts.kdata(kdata);
        charts.kdata_draw();
        if (p_check) {
          pdata_ajax(input_tick, input_stock_code, input_start_dt, input_end_dt);
        }
        if (sk_check) {
          skdata_ajax(input_tick, input_stock_code, input_start_dt, input_end_dt);
        }
        if (sg_check) {
          sgdata_ajax(input_tick, input_stock_code, input_start_dt, input_end_dt);
        }
      },
      error: function(err) {
        console.log("ajax error on query prices", err);
        charts.kdata_clear();
      }
    })
  });
  $("#chart_refresh").click(function() {
    charts.kdata_draw();
    charts.pdata_table();
    charts.skdata_table();
    charts.sgdata_table();
  });

});

