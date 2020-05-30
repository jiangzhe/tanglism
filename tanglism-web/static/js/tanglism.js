import { setupWebsocketEvents, validate_basic_cfg, draw } from './tanglism-draw.js';

$(document).ready(function() {
  // 股票选择
  $("#input_stock_code").autocomplete({
    source: function(req, callback) {
      $.ajax({
        url: "/api/keyword-stocks?keyword=" + encodeURIComponent(req.term),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          callback($.map(resp, function(item){
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
  // 时间选择
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
  $("#data_container").tabs();
  // 笔逻辑选择
  $("input[name='stroke_logic_gap']").click(function(e){
    var value = $(this).val();
    if (value === "gap_ratio") {
      $("#gap_ratio_percentage_span").css("display", "inline");
    } else {
      $("#gap_ratio_percentage_span").css("display", "none");
    }
  });
  // 柱间距选择
  $("input[name='bar_padding']").click(function(e){
    var value = $(this).val();
    if (value === "fixed") {
      $("#bar_padding_fixed_width_span").css("display", "inline");
    } else {
      $("#bar_padding_fixed_width_span").css("display", "none");
    }
  });
  // 画图事件
  $(".draw_trigger").change(function() {
    if (validate_basic_cfg()) {
      draw();
    }
  });

  // 标签页切换
  $("#tabs").tabs();

  // websocket
  var ws = new WebSocket("ws://" + location.hostname + ":" + location.port + "/ws/");
  setupWebsocketEvents(ws);
});

